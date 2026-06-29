#!/bin/sh
# Java 8 + Maven env setup for examples/java8-webapi
# Source this file: . ./.agents/scripts/setup-java-env.sh
export JAVA_HOME=/home/kudi/.local/jdk-install/jdk-8
export PATH="$JAVA_HOME/bin:/mnt/d/java/apache-maven-3.9.12/bin:$PATH"
export MAVEN_OPTS="-Xmx768m -Dfile.encoding=UTF-8"
# Verify
java -version
mvn -version | head -3
