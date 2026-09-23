if @cache[key]
  @cache[key]
else
  @cache[key] = heavy_load[key]
end
