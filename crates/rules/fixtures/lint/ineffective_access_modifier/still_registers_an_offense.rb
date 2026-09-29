class C

  private

  def instance_method
  end

  def self.method
  ^^^ `private` (on line 3) does not make singleton methods private. Use `private_class_method` or `private` inside a `class << self` block instead.
    puts "hi"
  end
end
