class Foo
  def self.x
    def self.y
    ^^^^^^^^^^ Method definitions must not be nested. Use `lambda` instead.
    end
  end
end
