Foo.configure do |c|
  def self.bar
    return if baz?
  end
end
