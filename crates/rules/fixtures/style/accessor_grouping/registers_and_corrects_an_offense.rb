class Foo
  attr_reader(
  ^^^^^^^^^^^^ Use one attribute per `attr_reader`.
    # comment one
    :one,
    # comment two A
    :two, # comment two B
    :three # comment three
  )
end
