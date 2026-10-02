class A
  FOO = :bar

  silence_warnings do
    FOO = :baz
  end
end
