class Foo
  def useful_instance_method
    do_something
  end

  def instance_method
  ^^^^^^^^^^^^^^^^^^^ Useless method definition detected.
    super
  end

  def instance_method_with_args(arg)
  ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Useless method definition detected.
    super(arg)
  end

  def self.useful_class_method
    do_something
  end

  def self.class_method
  ^^^^^^^^^^^^^^^^^^^^^ Useless method definition detected.
    super
  end

  def self.class_method_with_args(arg)
  ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Useless method definition detected.
    super(arg)
  end

  class << self
    def self.other_useful_class_method
      do_something
    end

    def other_class_method
    ^^^^^^^^^^^^^^^^^^^^^^ Useless method definition detected.
      super
    end

    def other_class_method_with_parens
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Useless method definition detected.
      super()
    end

    def other_class_method_with_args(arg)
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Useless method definition detected.
      super(arg)
    end
  end
end
