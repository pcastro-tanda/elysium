class A
  private_class_method def self.do_internal_work
  end

  public_class_method def self.do_something
  ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ `public_class_methods` is supposed to appear before `private_class_methods`.
  end
end
