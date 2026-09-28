case x
when a, b
  first_method
when b, a
        ^ Duplicate `when` condition detected.
     ^ Duplicate `when` condition detected.
  second_method
end
