module Foo
  def bar?(value)
    bar.instance_of?(::Bar)
  end

  class Bar
  end
end
