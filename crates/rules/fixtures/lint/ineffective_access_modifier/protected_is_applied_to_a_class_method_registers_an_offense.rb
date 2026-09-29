class C
  protected

  def self.method
  ^^^ `protected` (on line 2) does not make singleton methods protected. Use `protected` inside a `class << self` block instead.
    puts "hi"
  end
end
