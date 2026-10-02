case x
in foo: a, bar: b
  first_method
in bar: b, foo: a
   ^^^^^^^^^^^^^^ Duplicate `in` pattern detected.
  second_method
end
