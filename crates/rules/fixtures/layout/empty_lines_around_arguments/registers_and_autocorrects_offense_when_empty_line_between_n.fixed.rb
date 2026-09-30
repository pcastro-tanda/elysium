Foo.prepend(
  a,
  Module.new do
    def something; end

    def anything; end
  end
)
