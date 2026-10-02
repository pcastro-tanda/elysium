{}.reject { |x| x.match? /regexp/ }
{ foo: :bar }.reject { |x| x.match? /regexp/ }
