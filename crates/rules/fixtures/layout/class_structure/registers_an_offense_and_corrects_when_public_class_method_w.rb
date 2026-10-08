class Foo
  def instance_method
    'instance method'
  end

  def self.class_method
  ^^^^^^^^^^^^^^^^^^^^^ `public_class_methods` is supposed to appear before `public_methods`.
    <<~EOS
      class method
    EOS
  end
end
