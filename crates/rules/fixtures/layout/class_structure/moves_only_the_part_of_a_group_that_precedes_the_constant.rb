class Foo
  def do_something; end
  FIRST = 1
  ^^^^^^^^^ `constants` is supposed to appear before `public_methods`.
  DYNAMIC = do_something.freeze
  SECOND = 2
end
