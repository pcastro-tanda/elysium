case x
when false
  first_method
when true
  second_method
when false
     ^^^^^ Duplicate `when` condition detected.
  third_method
when true
     ^^^^ Duplicate `when` condition detected.
  fourth_method
end
