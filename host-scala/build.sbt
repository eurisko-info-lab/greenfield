ThisBuild / scalaVersion := "3.6.2"
ThisBuild / version := "0.1.0"
ThisBuild / organization := "greenfield"

lazy val root = (project in file("."))
  .settings(
    name := "stratum-scala",
    Compile / mainClass := Some("greenfield.Stratum")
  )
