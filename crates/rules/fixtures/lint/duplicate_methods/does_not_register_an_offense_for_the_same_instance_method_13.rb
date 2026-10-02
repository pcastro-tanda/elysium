Module.new { def test(*one, **two, &block) = super(one, two, yield(block)) }
      .instance_method(:test)
      .parameters
