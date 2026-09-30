users
  &.dup&.sort_by { _1.name.lower }
  &.page(params[:page])
