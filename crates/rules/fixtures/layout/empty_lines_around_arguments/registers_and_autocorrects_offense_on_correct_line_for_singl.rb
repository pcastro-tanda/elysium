class Foo

  include Bar

  def baz(qux)
    fizz(
      qux,

^{} Empty line detected around arguments.
      10
    )
  end
end
