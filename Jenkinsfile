pipeline {
        agent none
        stages {
                stage ('Build') {
                        parallel {
                                stage ('Linux CH/QEMU Tests') {
                                        agent { node { label 'focal-fw' } }
                                        options {
                                                timeout(time: 1, unit: 'HOURS')
                                        }
                                        stages {
                                                stage ('Checkout') {
                                                        steps {
                                                                checkout scm
                                                        }
                                                }
                                                stage ('Install system packages') {
                                                        steps {
                                                                sh "sudo apt-get -y install build-essential mtools qemu-system-x86 libssl-dev pkg-config"
                                                        }
                                                }
                                                stage ('Install Rust') {
                                                        steps {
                                                                sh "nohup curl https://sh.rustup.rs -sSf | sh -s -- -y"
                                                        }
                                                }
                                                stage('Run integration tests (Linux)') {
                                                          steps {
                                                                  sh "./run_integration_tests.sh linux"
                                                          }
                                                }
                                        }
                                }
                                stage ('Linux coreboot QEMU Tests') {
                                        agent { node { label 'focal-fw' } }
                                        options {
                                                timeout(time: 1, unit: 'HOURS')
                                        }
                                        stages {
                                                stage ('Checkout') {
                                                        steps {
                                                                checkout scm
                                                        }
                                                }
                                                stage ('Install system packages') {
                                                        steps {
                                                                sh "sudo apt-get -y install build-essential mtools qemu-system-x86 libssl-dev pkg-config m4 bison flex zlib1g-dev"
                                                        }
                                                }
                                                stage ('Install Rust') {
                                                        steps {
                                                                sh "nohup curl https://sh.rustup.rs -sSf | sh -s -- -y"
                                                        }
                                                }
                                                stage('Run integration tests') {
                                                          steps {
                                                                  sh "./run_coreboot_integration_tests.sh linux"
                                                          }
                                                }
                                        }
                                }
                                stage ('Windows CH Tests') {
                                        agent { node { label 'focal-fw' } }
                                        environment {
                                                AZURE_CONNECTION_STRING = credentials('46b4e7d6-315f-4cc1-8333-b58780863b9b')
                                        }
                                        options {
                                                timeout(time: 1, unit: 'HOURS')
                                        }
                                        stages {
                                                stage ('Checkout') {
                                                        steps {
                                                                checkout scm
                                                        }
                                                }
                                                stage ('Install system packages') {
                                                        steps {
                                                                sh "sudo apt-get -y install build-essential mtools qemu-system-x86 libssl-dev pkg-config azure-cli"
                                                        }
                                                }
                                                stage ('Install Rust') {
                                                        steps {
                                                                sh "nohup curl https://sh.rustup.rs -sSf | sh -s -- -y"
                                                        }
                                                }
                                                stage ('Download assets') {
                                                        steps {
                                                                sh "mkdir -p ./resources/images"
                                                                sh 'az storage blob download --container-name private-images --file "./resources/images/windows-server-2019.raw" --name windows-server-2019.raw --connection-string "$AZURE_CONNECTION_STRING"'
                                                        }
                                                }
                                                stage('Run integration tests') {
                                                          steps {
                                                                  sh "./run_integration_tests.sh windows"
                                                          }
                                                }
                                        }
                                }
                        }
                }
        }
}
