![Crate license: Apache 2.0 or MIT](https://badgen.net/badge/license/MIT)

# Log IP to File Service

_Windows Service to run in the background and log the current IP(s) to a file._

## Install
```pwsh
cargo install --git https://github.com/Nopvzutys/log_ip_to_file_service"
```

## Setup
```pwsh
ip_to_file -i
ip_to_file -l c:\ip_to_file.log.txt
ip_to_file -t 600
ip_to_file -o c:\ip.txt
Start-Service ip_to_file_service
```

## Uninstall
```pwsh
Stop-Service ip_to_file_service
ip_to_file -u
```
