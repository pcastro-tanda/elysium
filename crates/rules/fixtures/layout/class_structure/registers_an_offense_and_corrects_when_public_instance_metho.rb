class Foo
  def instance_method; end
  def self.class_method; end
  ^^^^^^^^^^^^^^^^^^^^^^^^^^ `public_class_methods` is supposed to appear before `public_methods`.
end
