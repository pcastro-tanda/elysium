# Bundler/InsecureProtocolSource

The source `:gemcutter`, `:rubygems` and `:rubyforge` are deprecated because HTTP requests are insecure. Please change your source to 'https://rubygems.org' if possible, or 'http://rubygems.org' if not.

| | |
| --- | --- |
| Department | Bundler |
| Enabled by default | true |
| Default severity | warning |
| Fix | safe |
| Stability | stable |



## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| AllowHttpProtocol | true |  | Allow `source 'http://rubygems.org'` for safe autocorrection. |

## Blind spots

None recorded.
