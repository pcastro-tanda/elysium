class A
  FOO = :bar

  silence_warnings do
    FOO = :baz
    BAR = :quux
  end
end
