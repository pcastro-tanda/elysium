class Foo
  def do_something
  end

  CONSTANT = <<~`EOS`
  ^^^^^^^^^^^^^^^^^^^ `constants` is supposed to appear before `public_methods`.
    str
  EOS
end
