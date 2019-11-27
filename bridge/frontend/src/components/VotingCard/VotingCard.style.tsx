import { makeStyles, Theme, colors } from 'utils/styles';

export const useStyles = makeStyles((theme: Theme) => ({
  root: {
    borderRadius: '0.25rem',
    backgroundColor: colors.white,
    boxShadow: '0px 1px 0px rgba(0, 0, 0, 0.1)',
  },

  mainInformation: {
    padding: theme.spacing(2),
  },

  metrics: {
    marginBottom: theme.spacing(2.5),
    marginRight: '1.5rem',
  },

  purple: {},
  grey: {},

  address: {
    fontSize: '1.25rem',
    lineHeight: '1.6',
  },

  voting: {
    borderLeft: `solid ${colors.athensGray} 1px`,
    padding: theme.spacing(2),
  },

  toggleExpandIcon: {
    marginRight: theme.spacing(1),
    color: colors.royalPurple,
    cursor: 'pointer',
  },

  expansionPanel: {
    boxShadow: 'none',
  },

  expansionPanelSummary: {
    padding: 0,
  },

  showLimits: {
    color: colors.royalPurple,
  },

  votingDecision: {
    padding: theme.spacing(1),
    borderRadius: '0.25rem',
    backgroundColor: colors.ghostWhite,
  },

  votingIcon: {
    width: '1.25rem',
    marginRight: theme.spacing(0.5),
  },

  votingForIcon: {
    marginRight: theme.spacing(1),
    composes: '$votingIcon',
    color: colors.shamrock,
  },

  votingAgainstIcon: {
    marginRight: theme.spacing(1),
    composes: '$votingIcon',
    color: colors.geraldine,
  },

  votingResult: {
    marginLeft: 'auto',
    padding: theme.spacing(2),
    borderLeft: `solid ${colors.athensGray} 1px`,
  },

  votingFor: {
    color: colors.shamrock,
  },

  votingAgainst: {
    color: colors.geraldine,
  },
}));
