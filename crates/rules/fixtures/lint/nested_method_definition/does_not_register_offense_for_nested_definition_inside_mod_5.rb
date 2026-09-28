class Foo
  def self.define
    Module.new do |m|
      def y
      end

      do_something(m)
    end
  end
end
