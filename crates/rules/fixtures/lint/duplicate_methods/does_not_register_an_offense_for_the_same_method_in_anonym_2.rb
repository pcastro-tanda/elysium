A&.prepend(
  Module.new do
    def foo
      x
    end
  end
)

B&.prepend(
  Module.new do
    def foo
      y
    end
  end
)
