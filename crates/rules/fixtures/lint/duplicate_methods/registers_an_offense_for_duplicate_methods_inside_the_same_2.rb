def build_klass
  Class.new do
    def name
      1
    end
    def name
    ^^^^^^^^ Method `Object#name` is defined at both (string):3 and (string):6.
      2
    end
  end
end
