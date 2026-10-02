let(:source) do
  <<~CODE
    func({
      @abc => 0,
      @xyz => 1
    })
    func(
      {
        abc: 0
      }
    )
    func(
      {},
      {
        xyz: 1
      }
    )
  CODE
end
