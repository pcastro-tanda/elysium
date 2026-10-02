dest = []
StringIO.new('foo:bar').each(':') { |e| dest << e }
