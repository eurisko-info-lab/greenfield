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
