pipeline {
        agent none
        stages {
                stage ('Build') {
                        parallel {
                                stage ('Linux guest Tests') {
                                        agent { node { label 'focal-fw' } }
                                        stages {
                                                stage ('Checkout') {
                                                        steps {
                                                                checkout scm
                                                        }
                                                }
                                                stage('Run unit tests') {
                                                          steps {
                                                                  sh "scripts/dev_cli.sh tests --unit"
                                                          }
                                                }
                                                stage('Run integration tests') {
                                                          options {
                                                                  timeout(time: 1, unit: 'HOURS')
                                                          }
                                                          steps {
                                                                  sh "scripts/dev_cli.sh tests --integration"
                                                          }
                                                }
                                                stage('Run coreboot integration tests') {
                                                          options {
                                                                  timeout(time: 1, unit: 'HOURS')
                                                          }
                                                          steps {
                                                                  sh "scripts/dev_cli.sh tests --integration-coreboot"
                                                          }
                                                }
                                        }
                                }
                                stage ('Windows guest Tests') {
                                        agent { node { label 'focal-fw' } }
                                        environment {
                                                AZURE_CONNECTION_STRING = credentials('46b4e7d6-315f-4cc1-8333-b58780863b9b')
                                        }
                                        stages {
                                                stage ('Checkout') {
                                                        steps {
                                                                checkout scm
                                                        }
                                                }
                                                stage ('Download assets') {
                                                        steps {
                                                                sh "sudo apt install -y azure-cli"
                                                                sh "mkdir -p ${env.HOME}/workloads"
                                                                sh 'az storage blob download --container-name private-images --file "$HOME/workloads/windows-server-2019.raw" --name windows-server-2019.raw --connection-string "$AZURE_CONNECTION_STRING"'
                                                        }
                                                }
                                                stage('Run Windows guest integration tests') {
                                                          options {
                                                                  timeout(time: 1, unit: 'HOURS')
                                                          }
                                                          steps {
                                                                  sh "scripts/dev_cli.sh tests --integration-windows"
                                                          }
                                                }
                                        }
                                }
                                stage ('AArch64 Unit Tests') {
                                        agent { node { label 'focal-arm64' } }
                                        stages {
                                                stage ('Checkout') {
                                                        steps {
                                                                checkout scm
                                                        }
                                                }
                                                stage('Run unit tests') {
                                                          steps {
                                                                  sh "scripts/dev_cli.sh --local tests --unit"
                                                          }
                                                }
                                        }
                                }
                        }
                }
        }
}
