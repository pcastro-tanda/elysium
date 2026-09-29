something.instance_eval do
  _1
  begin
    def bar=(bar)
      @bar = bar
    end
  end
end
