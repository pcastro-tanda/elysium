something.instance_eval do
  begin
    def bar
      @bar
    end
  end
end
