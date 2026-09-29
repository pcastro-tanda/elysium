STDOUT.puts('hello')
^^^^^^ Use `$stdout` instead of `STDOUT`.

hash = { out: STDOUT, key: value }
              ^^^^^^ Use `$stdout` instead of `STDOUT`.

def m(out = STDOUT)
            ^^^^^^ Use `$stdout` instead of `STDOUT`.
  out.puts('hello')
end
