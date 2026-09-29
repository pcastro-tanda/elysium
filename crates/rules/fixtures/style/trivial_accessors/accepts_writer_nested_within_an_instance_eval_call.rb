something.instance_eval do
  begin
    def bar=(bar)
      @bar = bar
    end
  end
end
