module MyRefinement
  refine(String) do
  ^^^^^^^^^^^^^^ Sorbet/Refinement: Do not use Ruby Refinements library as it is not supported by Sorbet.
    def to_s
      "foo"
    end
  end
end
