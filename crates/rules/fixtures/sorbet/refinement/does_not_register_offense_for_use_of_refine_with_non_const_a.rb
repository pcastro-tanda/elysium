module MyRefinement
  refine "foo" do
    def to_s
      "foo"
    end
  end
end
