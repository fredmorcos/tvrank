#include "titlesmodel.h"
#include <QColor>

TitlesModel::TitlesModel(QObject *parent): QAbstractTableModel {parent}
{}

void TitlesModel::setService(const struct TVrankService *service)
{
  service = service;
}

int TitlesModel::rowCount(const QModelIndex& /*parent*/) const
{
  return 3;
}

int TitlesModel::columnCount(const QModelIndex& /*parent*/) const
{
  return 10;
}

QVariant TitlesModel::data(const QModelIndex& index, int role) const
{
  if (role == Qt::DecorationRole && index.column() == 3) {
    // TODO return green, orange or red depending on rating
    return QColor {"blue"};
  }

  if (role != Qt::DisplayRole) {
    return QVariant();
  }

  return QString("Row%1, Column%2").arg(index.row() + 1).arg(index.column() + 1);
}

QVariant TitlesModel::headerData(int section, Qt::Orientation orientation, int role) const
{
  if (role != Qt::DisplayRole || orientation != Qt::Horizontal) {
    return QVariant();
  }

  switch (section) {
    case 0:
      return QStringLiteral("Primary Title");
    case 1:
      return QStringLiteral("Original Title");
    case 2:
      return QStringLiteral("Year");
    case 3:
      return QStringLiteral("Rating");
    case 4:
      return QStringLiteral("Votes");
    case 5:
      return QStringLiteral("Runtime");
    case 6:
      return QStringLiteral("Genres");
    case 7:
      return QStringLiteral("Type");
    case 8:
      return QStringLiteral("IMDB ID");
    case 9:
      return QStringLiteral("IMDB Link");
    default:
      return QVariant();
  }
}

Qt::ItemFlags TitlesModel::flags(const QModelIndex&) const
{
  return Qt::ItemIsEnabled | Qt::ItemIsSelectable | Qt::ItemNeverHasChildren;
}
