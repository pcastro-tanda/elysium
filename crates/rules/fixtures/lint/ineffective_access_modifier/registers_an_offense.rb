class C
  private

  def self.method2
  ^^^ `private` (on line 2) does not make singleton methods private. Use `private_class_method` or `private` inside a `class << self` block instead.
    puts "hi"
  end

  private_class_method :method
end
