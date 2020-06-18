pipeline {
        agent none
        stages {
                stage ("Worker build") {
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
                                stage ('Run integration tests') {
                                        steps {
                                                sh "./run_integration_tests.sh"
                                        }
                                }
                        }
                }
        }
}
