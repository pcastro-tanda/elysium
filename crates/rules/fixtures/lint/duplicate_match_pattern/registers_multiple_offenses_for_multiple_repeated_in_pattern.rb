case x
in foo
  first_method
in bar
  second_method
in foo
   ^^^ Duplicate `in` pattern detected.
  third_method
in bar
   ^^^ Duplicate `in` pattern detected.
  fourth_method
end
