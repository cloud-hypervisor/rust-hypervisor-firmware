pipeline {
        agent none
        stages {
                stage ('Build') {
                        parallel {
                                stage ('CH/QEMU Tests') {
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
                                                /*
                                                stage ('Download assets') {
                                                        steps {
                                                                sh "mkdir ./resources/images"
                                                                azureDownload(storageCredentialId: 'ch-image-store',
                                                                                          containerName: 'private-images',
                                                                                          includeFilesPattern: 'windows-server-2019.raw',
                                                                                          downloadType: 'container',
                                                                                          downloadDirLoc: "./resources/images")
                                                        }
                                                }
                                                */
                                                stage('Run integration tests') {
                                                          steps {
                                                                  sh "./run_integration_tests.sh"
                                                          }
                                                }
                                        }
                                }
                                stage ('coreboot QEMU Tests') {
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
                                                /*
                                                stage ('Download assets') {
                                                        steps {
                                                                sh "mkdir ./resources/images"
                                                                azureDownload(storageCredentialId: 'ch-image-store',
                                                                                          containerName: 'private-images',
                                                                                          includeFilesPattern: 'windows-server-2019.raw',
                                                                                          downloadType: 'container',
                                                                                          downloadDirLoc: "./resources/images")
                                                        }
                                                }
                                                */
                                                stage('Run integration tests') {
                                                          steps {
                                                                  sh "./run_coreboot_integration_tests.sh"
                                                          }
                                                }
                                        }
                                }
                        }
                }
        }
}
