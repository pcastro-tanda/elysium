foo.each do
  if it
    def m
      it&.bar
    end
  end
end
