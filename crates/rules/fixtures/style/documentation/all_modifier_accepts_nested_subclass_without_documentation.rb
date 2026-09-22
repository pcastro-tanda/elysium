module A #:nodoc: all
  module B
    TEST = 20
    class Test < Parent
      TEST = 20
    end
  end
end
