A.prepend(
  Module.new do
    def foo
      x
    end
  end
)

A.prepend(
  Module.new do
    def foo
    ^^^^^^^ Method `Object#foo` is defined at both (string):3 and (string):11.
      y
    end
  end
)
