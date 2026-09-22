some_method(
  <<-LOREM.delete("\n"),
                       ^ Avoid comma after the last parameter of a method call.
    Something with a , in it
  LOREM
)
