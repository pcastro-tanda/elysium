case x
when a && b
  first_method
when a && b
     ^^^^^^ Duplicate `when` condition detected.
  second_method
end
