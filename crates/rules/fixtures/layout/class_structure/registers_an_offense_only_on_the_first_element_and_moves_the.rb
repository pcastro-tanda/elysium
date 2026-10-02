class Foo
  def do_something; end
  FIRST = 1
  ^^^^^^^^^ `constants` is supposed to appear before `public_methods`.
  SECOND = 2
end
