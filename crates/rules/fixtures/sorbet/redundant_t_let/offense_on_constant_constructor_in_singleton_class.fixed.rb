class Foo
  class << self
    DEFAULT = Pathname.new("/x").freeze
  end
end
