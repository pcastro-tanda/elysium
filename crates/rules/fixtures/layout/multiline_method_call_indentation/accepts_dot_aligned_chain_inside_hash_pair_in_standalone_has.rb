{
  alpha_beta_count: @handler.retrieve_all_by(alpha: alpha)
                            .authorized.count,
  beta_count: @handler.retrieve_all.authorized.count
}
