package greenfield

import java.io.File
import java.nio.charset.StandardCharsets
import java.security.MessageDigest
import scala.collection.mutable.ArrayBuffer
import scala.io.Source
import scala.util.{Failure, Try}

sealed trait Value
case object UnitValue extends Value
case class BoolValue(value: Boolean) extends Value
case class NatValue(value: Long) extends Value
case class BytesValue(value: Array[Byte]) extends Value
case class TextValue(value: String) extends Value
case class SumValue(tag: Long, payload: Value) extends Value
case class ProductValue(items: Vector[Value]) extends Value
case class SequenceValue(items: Vector[Value]) extends Value
case class FiniteMapValue(entries: Vector[(Value, Value)]) extends Value
case class DigestValue(digest: Array[Byte]) extends Value
case class RefValue(digest: Array[Byte], typeDigest: Array[Byte]) extends Value

case class CanonError(message: String)

final class CasStore {
  private val entries = scala.collection.mutable.HashMap.empty[Array[Byte], Array[Byte]]

  def put(bytes: Array[Byte]): Array[Byte] = {
    val digest = Stratum.digestBytes(bytes)
    entries.update(digest, bytes.clone())
    digest
  }

  def get(digest: Array[Byte]): Option[Array[Byte]] = entries.get(digest).map(_.clone)
}

object Cas {
  private val store = new CasStore

  def put(bytes: Array[Byte]): Array[Byte] = store.put(bytes)

  def get(digest: Array[Byte]): Option[Array[Byte]] = store.get(digest)
}

sealed trait TypeDescription
case object TypeUnit extends TypeDescription
case object TypeBool extends TypeDescription
case object TypeNat extends TypeDescription
case object TypeBytes extends TypeDescription
case object TypeText extends TypeDescription
case object TypeDigest extends TypeDescription
case class TypeSum(left: TypeDescription, right: TypeDescription) extends TypeDescription
case class TypeProduct(items: Vector[TypeDescription]) extends TypeDescription
case class TypeSequence(itemType: TypeDescription) extends TypeDescription
case class TypeFiniteMap(keyType: TypeDescription, valueType: TypeDescription) extends TypeDescription
case object TypeRef extends TypeDescription
case class TypeArrow(inputType: TypeDescription, outputType: TypeDescription) extends TypeDescription

case class TypedNode(typeDigest: Array[Byte], value: Value)
case class Graph(entries: scala.collection.mutable.Map[Array[Byte], TypedNode] = scala.collection.mutable.HashMap.empty[Array[Byte], TypedNode]) {
  def insert(value: Value, typeDigest: Array[Byte]): Array[Byte] = {
    val digest = Stratum.digestBytes(Stratum.encodeValue(value).toOption.getOrElse(Array.emptyByteArray))
    entries.update(digest, TypedNode(typeDigest, value))
    digest
  }

  def get(digest: Array[Byte]): Option[TypedNode] = entries.get(digest)
}

sealed trait GraphError
case class MissingDigest(digest: Array[Byte]) extends GraphError
case object TypeMismatch extends GraphError
case object InvalidRef extends GraphError

object GraphOps {
  def typeDigestForValue(value: Value): Array[Byte] = value match {
    case UnitValue => Stratum.digestBytes("unit".getBytes(StandardCharsets.UTF_8))
    case BoolValue(_) => Stratum.digestBytes("bool".getBytes(StandardCharsets.UTF_8))
    case NatValue(_) => Stratum.digestBytes("nat".getBytes(StandardCharsets.UTF_8))
    case BytesValue(_) => Stratum.digestBytes("bytes".getBytes(StandardCharsets.UTF_8))
    case TextValue(_) => Stratum.digestBytes("text".getBytes(StandardCharsets.UTF_8))
    case SumValue(_, _) => Stratum.digestBytes("sum".getBytes(StandardCharsets.UTF_8))
    case ProductValue(_) => Stratum.digestBytes("product".getBytes(StandardCharsets.UTF_8))
    case SequenceValue(_) => Stratum.digestBytes("sequence".getBytes(StandardCharsets.UTF_8))
    case FiniteMapValue(_) => Stratum.digestBytes("finitemap".getBytes(StandardCharsets.UTF_8))
    case DigestValue(_) => Stratum.digestBytes("digest".getBytes(StandardCharsets.UTF_8))
    case RefValue(_, _) => Stratum.digestBytes("ref".getBytes(StandardCharsets.UTF_8))
  }

  def checkType(value: Value, ty: TypeDescription, graph: Graph): Either[GraphError, Unit] = {
    (value, ty) match {
      case (UnitValue, TypeUnit) => Right(())
      case (BoolValue(_), TypeBool) => Right(())
      case (NatValue(_), TypeNat) => Right(())
      case (BytesValue(_), TypeBytes) => Right(())
      case (TextValue(_), TypeText) => Right(())
      case (DigestValue(_), TypeDigest) => Right(())
      case (SumValue(_, payload), TypeSum(left, right)) =>
        checkType(payload, right, graph).flatMap(_ => checkType(UnitValue, left, graph))
      case (ProductValue(items), TypeProduct(types)) if items.length == types.length =>
        items.zip(types).foldLeft[Either[GraphError, Unit]](Right(())) {
          case (Right(_), (item, itemType)) => checkType(item, itemType, graph)
          case (Left(err), _) => Left(err)
        }
      case (SequenceValue(items), TypeSequence(itemType)) =>
        items.foldLeft[Either[GraphError, Unit]](Right(())) {
          case (Right(_), item) => checkType(item, itemType, graph)
          case (Left(err), _) => Left(err)
        }
      case (FiniteMapValue(entries), TypeFiniteMap(keyType, valueType)) =>
        entries.foldLeft[Either[GraphError, Unit]](Right(())) {
          case (Right(_), (key, value)) =>
            checkType(key, keyType, graph).flatMap(_ => checkType(value, valueType, graph))
          case (Left(err), _) => Left(err)
        }
      case (RefValue(digest, typeDigest), TypeRef) =>
        graph.get(digest).toRight(MissingDigest(digest)).flatMap { node =>
          if (java.util.Arrays.equals(node.typeDigest, typeDigest)) Right(()) else Left(TypeMismatch)
        }
      case (RefValue(digest, typeDigest), _) =>
        graph.get(digest).toRight(MissingDigest(digest)).flatMap { node =>
          if (java.util.Arrays.equals(node.typeDigest, typeDigest)) Right(()) else Left(TypeMismatch)
        }
      case _ => Left(TypeMismatch)
    }
  }

  def close(graph: Graph, rootDigest: Array[Byte]): Either[GraphError, Vector[Array[Byte]]] = {
    val seen = scala.collection.mutable.HashSet.empty[String]
    val stack = scala.collection.mutable.ArrayBuffer(rootDigest)
    val out = scala.collection.mutable.ArrayBuffer.empty[Array[Byte]]
    while (stack.nonEmpty) {
      val digest = stack.remove(stack.length - 1)
      val key = Stratum.hexEncode(digest)
      if (!seen.contains(key)) {
        seen.add(key)
        out += digest
        graph.get(digest) match {
          case Some(node) =>
            node.value match {
              case RefValue(childDigest, typeDigest) =>
                if (graph.get(childDigest).exists(_.typeDigest.sameElements(typeDigest))) {
                  stack += childDigest
                } else {
                  return Left(TypeMismatch)
                }
              case _ =>
            }
          case None => return Left(MissingDigest(digest))
        }
      }
    }
    Right(out.toVector)
  }
}

case class Arrow(
  inputType: TypeDescription,
  outputType: TypeDescription,
  coreModule: CoreModule,
  functionIndex: Int,
)

case class Lens(
  sourceType: TypeDescription,
  viewType: TypeDescription,
  getArrow: Arrow,
  modifyArrow: Arrow,
)

object ArrowOps {
  def runArrow(arrow: Arrow, input: Value): Verdict = Core.evalCore(arrow.coreModule, arrow.functionIndex, Vector(input))

  def runLensGet(lens: Lens, input: Value): Verdict = runArrow(lens.getArrow, input)

  def runLensModify(lens: Lens, input: Value, newView: Value): Verdict =
    runArrow(lens.modifyArrow, ProductValue(Vector(input, newView)))
}

sealed trait Delta
case object DeltaZero extends Delta
case class DeltaReplace(value: Value) extends Delta
case class DeltaProduct(items: Vector[Delta]) extends Delta
case class DeltaSum(tag: Long, delta: Delta) extends Delta
case class DeltaSequence(index: Int, value: Value) extends Delta
case class DeltaMapInsert(key: Value, value: Value) extends Delta
case class DeltaMapRemove(key: Value) extends Delta
case class DeltaBytesAppend(value: Array[Byte]) extends Delta

sealed trait DeltaError
case object DeltaTypeMismatch extends DeltaError
case object DeltaNotComposable extends DeltaError
case object InvalidDeltaIndex extends DeltaError
case class UnsupportedDelta(msg: String) extends DeltaError

object DeltaOps {
  def zeroForType(ty: TypeDescription): Delta = ty match {
    case TypeUnit | TypeBool | TypeNat | TypeBytes | TypeText | TypeDigest | TypeRef | TypeArrow(_, _) => DeltaZero
    case TypeProduct(items) => DeltaProduct(items.map(_ => DeltaZero))
    case TypeSequence(_) => DeltaZero
    case TypeFiniteMap(_, _) => DeltaZero
    case TypeSum(_, _) => DeltaZero
  }

  def applyDelta(ty: TypeDescription, value: Value, delta: Delta): Either[DeltaError, Value] =
    (ty, value, delta) match {
      case (_, _, DeltaZero) => Right(value)
      case (_, _, DeltaReplace(next)) => Right(next)
      case (TypeProduct(types), ProductValue(items), DeltaProduct(deltas)) if items.length == types.length && deltas.length == types.length =>
        items.zip(types).zip(deltas).foldLeft[Either[DeltaError, Vector[Value]]](Right(Vector.empty)) {
          case (Right(acc), ((item, itemType), itemDelta)) => applyDelta(itemType, item, itemDelta).map(acc :+ _)
          case (Left(err), _) => Left(err)
        }.map(ProductValue(_))
      case (TypeSequence(_), SequenceValue(items), DeltaSequence(index, replacement)) if index >= 0 && index < items.length =>
        val next = items.updated(index, replacement)
        Right(SequenceValue(next))
      case (TypeFiniteMap(_, _), FiniteMapValue(entries), DeltaMapInsert(key, value)) =>
        val updated = entries.indexWhere { case (existing, _) => existing == key }
        if (updated >= 0) {
          val next = entries.updated(updated, key -> value)
          Right(FiniteMapValue(next))
        } else {
          Right(FiniteMapValue(entries :+ (key -> value)))
        }
      case (TypeFiniteMap(_, _), FiniteMapValue(entries), DeltaMapRemove(key)) =>
        Right(FiniteMapValue(entries.filterNot { case (existing, _) => existing == key }))
      case (TypeBytes, BytesValue(bytes), DeltaBytesAppend(extra)) =>
        Right(BytesValue(bytes ++ extra))
      case _ => Left(DeltaTypeMismatch)
    }

  def diffDelta(ty: TypeDescription, before: Value, after: Value): Either[DeltaError, Delta] =
    if (before == after) Right(DeltaZero)
    else (ty, before, after) match {
      case (TypeProduct(types), ProductValue(beforeItems), ProductValue(afterItems)) if beforeItems.length == afterItems.length && beforeItems.length == types.length =>
        val deltas = beforeItems.zip(afterItems).zip(types).map { case ((b, a), t) => diffDelta(t, b, a) }
        val combined = deltas.foldLeft[Either[DeltaError, Vector[Delta]]](Right(Vector.empty)) {
          case (Right(acc), Right(delta)) => Right(acc :+ delta)
          case (Left(err), _) => Left(err)
          case (_, Left(err)) => Left(err)
        }
        combined.map(DeltaProduct(_))
      case (TypeSequence(_), SequenceValue(beforeItems), SequenceValue(afterItems)) if beforeItems.length == afterItems.length =>
        val deltas = beforeItems.zip(afterItems).zipWithIndex.map { case ((_, a), idx) => DeltaSequence(idx, a) }
        Right(DeltaProduct(deltas.toVector))
      case (TypeFiniteMap(_, _), FiniteMapValue(beforeEntries), FiniteMapValue(afterEntries)) =>
        val inserts = afterEntries.collect {
          case (key, value) if !beforeEntries.exists { case (existing, _) => existing == key } || beforeEntries.find { case (existing, _) => existing == key }.exists(_._2 != value) => DeltaMapInsert(key, value)
        }
        val removes = beforeEntries.collect {
          case (key, _) if !afterEntries.exists { case (existing, _) => existing == key } => DeltaMapRemove(key)
        }
        val result = inserts ++ removes
        if (result.isEmpty) Right(DeltaZero) else Right(DeltaProduct(result.toVector))
      case _ => Right(DeltaReplace(after))
    }

  def composeDelta(ty: TypeDescription, left: Delta, right: Delta): Either[DeltaError, Delta] =
    (left, right) match {
      case (DeltaZero, _) => Right(right)
      case (_, DeltaZero) => Right(left)
      case (DeltaReplace(_), DeltaReplace(next)) => Right(DeltaReplace(next))
      case (DeltaProduct(xs), DeltaProduct(ys)) if xs.length == ys.length =>
        val combined = xs.zip(ys).foldLeft[Either[DeltaError, Vector[Delta]]](Right(Vector.empty)) {
          case (Right(acc), (x, y)) => composeDelta(ty, x, y).map(acc :+ _)
          case (Left(err), _) => Left(err)
        }
        combined.map(DeltaProduct(_))
      case _ => Left(DeltaNotComposable)
    }
}

sealed trait Operation
case object NoopOperation extends Operation
case class SetNatOperation(value: Long) extends Operation
case class AddNatOperation(delta: Long) extends Operation
case class SetTextOperation(value: String) extends Operation
case class MapInsertOperation(key: Value, value: Value) extends Operation
case class SequencePushOperation(value: Value) extends Operation

case class OperationLanguage(operationType: TypeDescription, stateType: TypeDescription)

object OperationLanguageOps {
  def elaborate(language: OperationLanguage, state: Value, operation: Operation): Either[DeltaError, Delta] =
    (language.stateType, state, operation) match {
      case (TypeNat, _, SetNatOperation(value)) => Right(DeltaReplace(NatValue(value)))
      case (TypeNat, NatValue(current), AddNatOperation(delta)) => Right(DeltaReplace(NatValue(current + delta)))
      case (TypeText, _, SetTextOperation(value)) => Right(DeltaReplace(TextValue(value)))
      case (TypeFiniteMap(_, _), FiniteMapValue(_), MapInsertOperation(key, value)) => Right(DeltaMapInsert(key, value))
      case (TypeSequence(_), SequenceValue(_), SequencePushOperation(value)) => Right(DeltaSequence(0, value))
      case _ => Left(UnsupportedDelta("operation unsupported for state type"))
    }

  def reachable(genesis: Value, operations: Vector[Operation], depth: Int): Either[DeltaError, Vector[Value]] = {
    var current = genesis
    val states = scala.collection.mutable.ArrayBuffer(current)
    var count = 0
    while (count < depth && count < operations.length) {
      val language = OperationLanguage(TypeNat, TypeNat)
      val delta = elaborate(language, current, operations(count)).flatMap(DeltaOps.applyDelta(language.stateType, current, _))
      delta match {
        case Right(next) =>
          current = next
          states += current
        case Left(err) => return Left(err)
      }
      count += 1
    }
    Right(states.toVector)
  }
}

case class OpNode(languageDigest: Array[Byte], operationDigest: Array[Byte], dependencies: Vector[Array[Byte]])
case class History(nodes: Vector[OpNode])
case class Materialization(genesisDigest: Array[Byte], frontier: Vector[Array[Byte]], stateDigest: Array[Byte])
case class Conflict(nodes: Vector[Array[Byte]])

case class AccelManifest(
  semanticArrow: Arrow,
  sourceClosureDigest: Array[Byte],
  targetKind: String,
  implementationDigest: Array[Byte],
)

case class AccelBinding(manifestDigest: Array[Byte], credentialDigest: Option[Array[Byte]])

object AccelRegistry {
  private val registry = scala.collection.mutable.HashMap.empty[Array[Byte], (Value, Budget) => Verdict]

  def register(implementationDigest: Array[Byte], implementation: (Value, Budget) => Verdict): Unit =
    registry.update(implementationDigest, implementation)

  def run(manifest: AccelManifest, input: Value, budget: Budget): Verdict =
    registry.get(manifest.implementationDigest).map(_(input, budget)).getOrElse(ArrowOps.runArrow(manifest.semanticArrow, input))
}

object HistoryOps {
  def frontier(history: History): Vector[Array[Byte]] =
    history.nodes.map(node => Stratum.digestBytes(Stratum.encodeValue(TextValue(node.toString)).toOption.getOrElse(Array.emptyByteArray)))

  def materialize(history: History, genesis: Value, state: Value): Materialization = {
    val genesisDigest = Stratum.digestBytes(Stratum.encodeValue(genesis).toOption.getOrElse(Array.emptyByteArray))
    val stateDigest = Stratum.digestBytes(Stratum.encodeValue(state).toOption.getOrElse(Array.emptyByteArray))
    Materialization(genesisDigest, frontier(history), stateDigest)
  }

  def detectConflict(history: History): Option[Conflict] =
    if (history.nodes.length < 2) None
    else Some(Conflict(history.nodes.map(node => Stratum.digestBytes(Stratum.encodeValue(TextValue(node.toString)).toOption.getOrElse(Array.emptyByteArray)))))
}

sealed trait CoreExpr
case class LiteralExpr(value: Value) extends CoreExpr
case class ArgumentExpr(index: Long) extends CoreExpr
case class LocalExpr(index: Long) extends CoreExpr
case class LetExpr(left: CoreExpr, right: CoreExpr) extends CoreExpr
case class ProductExpr(items: Vector[CoreExpr]) extends CoreExpr
case class SumExpr(tag: Long, payload: CoreExpr) extends CoreExpr
case class MatchExpr(scrutinee: CoreExpr, arms: Vector[CoreExpr]) extends CoreExpr
case class PrimitiveExpr(name: String, args: Vector[CoreExpr]) extends CoreExpr
case class CallExpr(index: Long, args: Vector[CoreExpr]) extends CoreExpr
case class EffectExpr(name: String, payload: CoreExpr) extends CoreExpr

case class CoreFunction(arity: Long, body: CoreExpr)
case class CoreModule(functions: Vector[CoreFunction])

case class Receipt(
  capabilityDigest: Array[Byte],
  handlerDigest: Array[Byte],
  requestDigest: Array[Byte],
  responseDigest: Array[Byte],
  status: String,
)

sealed trait Outcome
case class Returned(value: Value) extends Outcome
case class Failed(msg: String) extends Outcome
case class Exhausted(kind: String) extends Outcome

case class Budget(maxSteps: Long, maxDepth: Long, maxAlloc: Option[Long] = None)
case class Verdict(outcome: Outcome, steps: Long, receipts: Vector[Receipt])

object Core {
  private val MaxDepth = 32L

  def casPut(bytes: Array[Byte]): Array[Byte] = Cas.put(bytes)

  def casGet(digest: Array[Byte]): Option[Array[Byte]] = Cas.get(digest)

  def evalCore(module: CoreModule, functionIndex: Int, args: Vector[Value]): Verdict = {
    val budget = Budget(64L, MaxDepth)
    val initialSteps = budget.maxSteps
    val receipts = scala.collection.mutable.ArrayBuffer.empty[Receipt]
    val outcome = evalExpr(module.functions(functionIndex).body, args, module, budget, receipts) match {
      case Right(value) => Returned(value)
      case Left(err) => err
    }
    val consumed = initialSteps - budget.maxSteps
    Verdict(outcome, consumed, receipts.toVector)
  }

  private def evalExpr(
    expr: CoreExpr,
    env: Vector[Value],
    module: CoreModule,
    budget: Budget,
    receipts: scala.collection.mutable.ArrayBuffer[Receipt],
  ): Either[Outcome, Value] = {
    if (budget.maxSteps == 0) return Left(Exhausted("max_steps"))
    val nextBudget = budget.copy(maxSteps = budget.maxSteps - 1)

    expr match {
      case LiteralExpr(value) => Right(value)
      case ArgumentExpr(index) =>
        env.lift(index.toInt).toRight(Failed(s"missing arg $index"))
      case LocalExpr(index) =>
        env.lift(index.toInt).toRight(Failed(s"missing local $index"))
      case LetExpr(left, right) =>
        for {
          value <- evalExpr(left, env, module, nextBudget, receipts)
          next = env :+ value
          result <- evalExpr(right, next, module, nextBudget, receipts)
        } yield result
      case ProductExpr(items) =>
        val values = items.iterator.foldLeft(Right(Vector.empty[Value]): Either[Outcome, Vector[Value]]) {
          case (Right(acc), item) => evalExpr(item, env, module, nextBudget, receipts).map(acc :+ _)
          case (Left(err), _) => Left(err)
        }
        values.map(ProductValue(_))
      case SumExpr(tag, payload) =>
        evalExpr(payload, env, module, nextBudget, receipts).map(SumValue(tag, _))
      case MatchExpr(scrutinee, arms) =>
        evalExpr(scrutinee, env, module, nextBudget, receipts).flatMap {
          case SumValue(tag, payload) =>
            arms.lift(tag.toInt).toRight(Failed("no match arm")).flatMap(expr =>
              evalExpr(expr, env :+ payload, module, nextBudget, receipts)
            )
          case _ => Left(Failed("non-sum match"))
        }
      case PrimitiveExpr(name, args) => evalPrimitive(name, args, env, module, nextBudget, receipts)
      case CallExpr(index, args) =>
        val function = module.functions.lift(index.toInt).toRight(Failed("unknown call"))
        function.flatMap { fn =>
          if (fn.arity != args.length.toLong) Left(Failed("arity mismatch"))
          else {
            val callArgs = args.iterator.foldLeft(Right(Vector.empty[Value]): Either[Outcome, Vector[Value]]) {
              case (Right(acc), arg) => evalExpr(arg, env, module, nextBudget, receipts).map(acc :+ _)
              case (Left(err), _) => Left(err)
            }
            callArgs.flatMap { values =>
              if (nextBudget.maxDepth == 0) Left(Exhausted("max_depth"))
              else evalExpr(fn.body, values, module, nextBudget.copy(maxDepth = nextBudget.maxDepth - 1), receipts)
            }
          }
        }
      case EffectExpr(name, payload) =>
        val request = evalExpr(payload, env, module, nextBudget, receipts)
        request.map { req =>
          val response = name match {
            case "hash" =>
              val encoded = Stratum.encodeValue(req).toOption.getOrElse(Array.emptyByteArray)
              DigestValue(Stratum.digestBytes(encoded))
            case "cas_get" =>
              req match {
                case DigestValue(digest) => Cas.get(digest).map(BytesValue(_)).getOrElse(UnitValue)
                case _ => UnitValue
              }
            case "cas_put" =>
              val bytes = req match {
                case BytesValue(bs) => bs
                case other => Stratum.encodeValue(other).toOption.getOrElse(Array.emptyByteArray)
              }
              val digest = Cas.put(bytes)
              DigestValue(digest)
            case "log_trace" => TextValue(s"trace:$name")
            case _ => TextValue(s"effect:$name")
          }
          val requestBytes = Stratum.encodeValue(req).toOption.getOrElse(Array.emptyByteArray)
          val responseBytes = Stratum.encodeValue(response).toOption.getOrElse(Array.emptyByteArray)
          receipts += Receipt(
            Stratum.digestBytes(name.getBytes(StandardCharsets.UTF_8)),
            Stratum.digestBytes(s"$name:handler".getBytes(StandardCharsets.UTF_8)),
            Stratum.digestBytes(requestBytes),
            Stratum.digestBytes(responseBytes),
            "ok"
          )
          response
        }
    }
  }

  private def evalPrimitive(
    name: String,
    args: Vector[CoreExpr],
    env: Vector[Value],
    module: CoreModule,
    budget: Budget,
    receipts: scala.collection.mutable.ArrayBuffer[Receipt],
  ): Either[Outcome, Value] = {
    val values = args.iterator.foldLeft(Right(Vector.empty[Value]): Either[Outcome, Vector[Value]]) {
      case (Right(acc), arg) => evalExpr(arg, env, module, budget, receipts).map(acc :+ _)
      case (Left(err), _) => Left(err)
    }

    values.flatMap {
      case Vector(NatValue(a), NatValue(b)) if name == "nat_add" =>
        if (a > 0 && b > Long.MaxValue - a || a < 0 && b < Long.MinValue - a) Left(Failed("nat_add overflow"))
        else Right(NatValue(a + b))
      case Vector(NatValue(a), NatValue(b)) if name == "nat_mul" =>
        if (a != 0 && (b > Long.MaxValue / a || b < Long.MinValue / a)) Left(Failed("nat_mul overflow"))
        else Right(NatValue(a * b))
      case Vector(BoolValue(a), BoolValue(b)) if name == "bool_and" => Right(BoolValue(a && b))
      case Vector(BoolValue(a), BoolValue(b)) if name == "bool_or" => Right(BoolValue(a || b))
      case Vector(BytesValue(a), BytesValue(b)) if name == "eq_bytes" => Right(BoolValue(java.util.Arrays.equals(a, b)))
      case Vector(BytesValue(a), BytesValue(b)) if name == "bytes_concat" => Right(BytesValue(a ++ b))
      case Vector(BytesValue(bytes)) if name == "len_bytes" => Right(NatValue(bytes.length.toLong))
      case Vector(TextValue(a), TextValue(b)) if name == "eq_text" => Right(BoolValue(a == b))
      case Vector(TextValue(text)) if name == "len_text" => Right(NatValue(text.length.toLong))
      case Vector(DigestValue(a), DigestValue(b)) if name == "eq_digest" => Right(BoolValue(java.util.Arrays.equals(a, b)))
      case _ => Left(Failed(s"unknown primitive $name"))
    }
  }
}

object Stratum {
  private val MaxDepth = 64

  def main(args: Array[String]): Unit = {
    if (args.length < 2 || args(0) != "fixture") {
      System.err.println("usage: stratum-scala fixture <fixture-file>")
      sys.exit(1)
    }

    val file = new File(args(1))
    val text = Try(Source.fromFile(file, "UTF-8").mkString).recover { case err: Throwable =>
      System.err.println(s"failed to read fixture: $err")
      sys.exit(1)
      throw err
    }.get

    val fixtureId = file.getName.stripSuffix(".txt").stripSuffix(".json").stripSuffix(".manifest")
    val value = parseFixtureText(text).getOrElse {
      System.err.println(s"unsupported fixture format: $text")
      sys.exit(1)
    }

    val encoded = encodeValue(value) match {
      case Right(bytes) => bytes
      case Left(err) =>
        System.err.println(s"failed to encode fixture: ${err.message}")
        sys.exit(1)
    }

    val digest = digestBytes(encoded)
    val outcomeDigest = digestBytes(hexEncode(digest).getBytes(StandardCharsets.UTF_8))
    println(
      s"{\"fixture_id\":\"$fixtureId\",\"encoded_hex\":\"${hexEncode(encoded)}\",\"digest_hex\":\"${hexEncode(digest)}\",\"outcome_digest\":\"${hexEncode(outcomeDigest)}\",\"steps\":1,\"receipt_digests\":[]}"
    )
  }

  def digestBytes(bytes: Array[Byte]): Array[Byte] = MessageDigest.getInstance("SHA-256").digest(bytes)

  def hexEncode(bytes: Array[Byte]): String = bytes.map(b => f"${b & 0xff}%02x").mkString

  def parseFixtureText(text: String): Option[Value] = {
    val input = text.trim
    if (input.isEmpty) return None

    if (input.startsWith("nat=")) {
      input.stripPrefix("nat=").toLongOption.map(NatValue)
    } else if (input.startsWith("text=")) {
      Some(TextValue(input.stripPrefix("text=")))
    } else if (input.startsWith("bool=")) {
      Some(BoolValue(input.stripPrefix("bool=") == "true"))
    } else if (input.startsWith("bytes=")) {
      val raw = input.stripPrefix("bytes=")
      val bytes = raw.grouped(2).toArray.flatMap(hex => Array(Integer.parseInt(hex, 16).toByte))
      Some(BytesValue(bytes))
    } else {
      None
    }
  }

  def encodeValue(value: Value, depth: Int = 0): Either[CanonError, Array[Byte]] = {
    if (depth > MaxDepth) return Left(CanonError("depth limit"))

    value match {
      case UnitValue => Right(Array(0.toByte))
      case BoolValue(flag) => Right(Array(1.toByte, if (flag) 1.toByte else 0.toByte))
      case NatValue(n) => encodeU64(n).map(bytes => Array(2.toByte) ++ bytes)
      case BytesValue(bytes) =>
        encodeU64(bytes.length.toLong).map(lenBytes => Array(3.toByte) ++ lenBytes ++ bytes)
      case TextValue(text) =>
        val bytes = text.getBytes(StandardCharsets.UTF_8)
        if (new String(bytes, StandardCharsets.UTF_8) == text) {
          encodeU64(bytes.length.toLong).map(lenBytes => Array(4.toByte) ++ lenBytes ++ bytes)
        } else {
          Left(CanonError("invalid utf-8"))
        }
      case SumValue(tag, payload) =>
        for {
          tagBytes <- encodeU64(tag)
          payloadBytes <- encodeValue(payload, depth + 1)
        } yield Array(5.toByte) ++ tagBytes ++ payloadBytes
      case ProductValue(items) => encodeSequence(items, 6, depth)
      case SequenceValue(items) => encodeSequence(items, 7, depth)
      case FiniteMapValue(entries) =>
        val sorted = entries.sortWith { case ((left, _), (right, _)) =>
          val leftBytes = encodeValue(left, depth + 1).toOption.getOrElse(Array.emptyByteArray)
          val rightBytes = encodeValue(right, depth + 1).toOption.getOrElse(Array.emptyByteArray)
          java.util.Arrays.compareUnsigned(leftBytes, rightBytes) < 0
        }

        val seen = scala.collection.mutable.HashSet.empty[String]
        for ((key, _) <- sorted) {
          val keyBytes = encodeValue(key, depth + 1).toOption.getOrElse(Array.emptyByteArray)
          if (!seen.add(hexEncode(keyBytes))) {
            return Left(CanonError("duplicate map key"))
          }
        }

        val payload = ArrayBuffer.empty[Byte]
        for ((key, value) <- sorted) {
          payload ++= encodeValue(key, depth + 1).toOption.getOrElse(Array.emptyByteArray)
          payload ++= encodeValue(value, depth + 1).toOption.getOrElse(Array.emptyByteArray)
        }

        encodeU64(sorted.length.toLong).map(lenBytes => Array(8.toByte) ++ lenBytes ++ payload.toArray)
      case DigestValue(digest) if digest.length == 32 => Right(Array(9.toByte) ++ digest)
      case DigestValue(_) => Left(CanonError("digest must be 32 bytes"))
      case RefValue(digest, typeDigest) if digest.length == 32 && typeDigest.length == 32 =>
        Right(Array(10.toByte) ++ digest ++ typeDigest)
      case RefValue(_, _) => Left(CanonError("ref digests must be 32 bytes"))
    }
  }

  private def encodeSequence(items: Vector[Value], tag: Int, depth: Int): Either[CanonError, Array[Byte]] = {
    val payload = ArrayBuffer.empty[Byte]
    for (item <- items) {
      encodeValue(item, depth + 1) match {
        case Right(bytes) => payload ++= bytes
        case Left(err) => return Left(err)
      }
    }
    encodeU64(items.length.toLong).map(lenBytes => Array(tag.toByte) ++ lenBytes ++ payload.toArray)
  }

  def decodeValue(bytes: Array[Byte], depth: Int = 0): Either[CanonError, Value] = {
    decodeValueAt(bytes, 0, depth) match {
      case Right((value, end)) if end == bytes.length => Right(value)
      case Right((_, end)) => Left(CanonError(s"trailing bytes at $end"))
      case Left(err) => Left(err)
    }
  }

  private def decodeValueAt(data: Array[Byte], pos: Int, depth: Int): Either[CanonError, (Value, Int)] = {
    if (depth > MaxDepth) return Left(CanonError("depth limit"))
    if (pos >= data.length) return Left(CanonError("unexpected end"))

    val tag = data(pos) & 0xff
    tag match {
      case 0x00 => Right((UnitValue, pos + 1))
      case 0x01 =>
        if (pos + 1 >= data.length) Left(CanonError("missing bool payload"))
        else Right((BoolValue(data(pos + 1) != 0), pos + 2))
      case 0x02 =>
        decodeU64(data, pos + 1).map { case (value, end) => (NatValue(value), end) }
      case 0x03 =>
        decodeU64(data, pos + 1).flatMap { case (len, next) =>
          val end = next + len.toInt
          if (end > data.length) Left(CanonError("bytes overflow"))
          else Right((BytesValue(java.util.Arrays.copyOfRange(data, next, end)), end))
        }
      case 0x04 =>
        decodeU64(data, pos + 1).flatMap { case (len, next) =>
          val end = next + len.toInt
          if (end > data.length) Left(CanonError("text overflow"))
          else {
            val text = new String(java.util.Arrays.copyOfRange(data, next, end), StandardCharsets.UTF_8)
            if (text.getBytes(StandardCharsets.UTF_8).sameElements(java.util.Arrays.copyOfRange(data, next, end))) {
              Right((TextValue(text), end))
            } else {
              Left(CanonError("invalid utf-8"))
            }
          }
        }
      case 0x05 =>
        decodeU64(data, pos + 1).flatMap { case (tagValue, next) =>
          decodeValueAt(data, next, depth + 1).map { case (payload, end) => (SumValue(tagValue, payload), end) }
        }
      case 0x06 => decodeCollection(data, pos + 1, isProduct = true, depth)
      case 0x07 => decodeCollection(data, pos + 1, isProduct = false, depth)
      case 0x08 => decodeMap(data, pos + 1, depth)
      case 0x09 =>
        val end = pos + 33
        if (end > data.length) Left(CanonError("digest overflow"))
        else Right((DigestValue(java.util.Arrays.copyOfRange(data, pos + 1, end)), end))
      case 0x0A =>
        val end = pos + 65
        if (end > data.length) Left(CanonError("ref overflow"))
        else {
          val digest = java.util.Arrays.copyOfRange(data, pos + 1, pos + 33)
          val typeDigest = java.util.Arrays.copyOfRange(data, pos + 33, end)
          Right((RefValue(digest, typeDigest), end))
        }
      case other => Left(CanonError(s"invalid tag $other"))
    }
  }

  private def decodeCollection(data: Array[Byte], pos: Int, isProduct: Boolean, depth: Int): Either[CanonError, (Value, Int)] = {
    decodeU64(data, pos).flatMap { case (count, next) =>
      var cursor = next
      val items = Vector.newBuilder[Value]
      var i = 0
      while (i < count.toInt) {
        decodeValueAt(data, cursor, depth + 1) match {
          case Right((value, end)) =>
            items += value
            cursor = end
            i += 1
          case Left(err) => return Left(err)
        }
      }
      val result = items.result()
      if (isProduct) Right((ProductValue(result), cursor)) else Right((SequenceValue(result), cursor))
    }
  }

  private def decodeMap(data: Array[Byte], pos: Int, depth: Int): Either[CanonError, (Value, Int)] = {
    decodeU64(data, pos).flatMap { case (count, next) =>
      var cursor = next
      val entries = Vector.newBuilder[(Value, Value)]
      val seen = scala.collection.mutable.HashSet.empty[String]
      var i = 0
      while (i < count.toInt) {
        decodeValueAt(data, cursor, depth + 1) match {
          case Right((key, afterKey)) =>
            val keyBytes = encodeValue(key, depth + 1).toOption.getOrElse(Array.emptyByteArray)
            val keyHex = hexEncode(keyBytes)
            if (!seen.add(keyHex)) return Left(CanonError("duplicate map key"))
            decodeValueAt(data, afterKey, depth + 1) match {
              case Right((value, afterValue)) =>
                entries += key -> value
                cursor = afterValue
              case Left(err) => return Left(err)
            }
          case Left(err) => return Left(err)
        }
        i += 1
      }
      Right((FiniteMapValue(entries.result()), cursor))
    }
  }

  def encodeU64(value: Long): Either[CanonError, Array[Byte]] = {
    if (value < 0) return Left(CanonError("negative value cannot be encoded"))

    val out = ArrayBuffer.empty[Byte]
    var current = value
    var continue = true
    while (continue) {
      var byte = (current & 0x7f).toByte
      current = current >>> 7
      if (current != 0) {
        byte = (byte | 0x80).toByte
      }
      out += byte
      if (current == 0) {
        continue = false
      }
    }
    Right(out.toArray)
  }

  def decodeU64(data: Array[Byte], pos: Int): Either[CanonError, (Long, Int)] = {
    var cursor = pos
    var value: Long = 0L
    var shift = 0
    var loops = 0
    while (loops < 10) {
      if (cursor >= data.length) return Left(CanonError("varint truncated"))
      val byte = data(cursor) & 0xff
      cursor += 1
      value |= ((byte & 0x7f).toLong << shift)
      if ((byte & 0x80) == 0) {
        val canonical = encodeU64(value)
        val consumed = java.util.Arrays.copyOfRange(data, pos, cursor)
        if (canonical.exists(arr => java.util.Arrays.equals(arr, consumed))) {
          return Right((value, cursor))
        }
        return Left(CanonError("non-canonical varint"))
      }
      shift += 7
      loops += 1
    }
    Left(CanonError("non-canonical varint"))
  }
}
