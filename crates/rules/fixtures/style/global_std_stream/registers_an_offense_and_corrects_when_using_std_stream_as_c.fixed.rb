$stdout.puts('hello')

hash = { out: $stdout, key: value }

def m(out = $stdout)
  out.puts('hello')
end
