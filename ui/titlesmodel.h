#pragma once

#include "tvrank.h"
#include <QAbstractTableModel>

class TitlesModel: public QAbstractTableModel
{
  Q_OBJECT

  const struct TVrankService *service = nullptr;

 public:
  explicit TitlesModel(QObject *parent = nullptr);

  void setService(const struct TVrankService *service);

  int rowCount(const QModelIndex& parent = QModelIndex()) const override;
  int columnCount(const QModelIndex& parent = QModelIndex()) const override;
  QVariant data(const QModelIndex& index, int role = Qt::DisplayRole) const override;
  QVariant
  headerData(int section, Qt::Orientation orientation, int role = Qt::DisplayRole) const override;
  Qt::ItemFlags flags(const QModelIndex& index) const override;
};
