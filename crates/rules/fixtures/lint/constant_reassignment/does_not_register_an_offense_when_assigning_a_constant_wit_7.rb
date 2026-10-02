module A
  module B
    FOO = :bar
  end

  ::B::FOO = :baz
end
