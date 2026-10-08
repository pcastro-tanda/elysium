class Foo
  def self.class_method
    <<~EOS
      class method
    EOS
  end
  def instance_method
    'instance method'
  end

end
